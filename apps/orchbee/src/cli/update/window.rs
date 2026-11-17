//! The window part (AU § How the window updates, § Installing a window version, § macOS window
//! versions). The window store `<store root>/desktop/` is a second store root with its own
//! `active.json`, `install.lock` and `versions/<version>/`, each version a window directory that
//! its `desktop-files.json` lists. `update` and the automatic helper run [`update`] after their
//! runtime part; `--installer-window-v1` runs [`installer`] on the window directory that Setup or
//! install.sh unpacked. Both end in one install: under the store root's `install.lock`, then
//! `desktop/install.lock`, after `download.lock` when a download came first, it copies exactly the
//! listed files into a private stage, each checked by size and SHA-256 as it is written and then
//! flushed, gives a file the list marks executable mode 0700 and every other 0600, makes each
//! listed link, publishes the stage without replacing, and moves the window pointer. A stored
//! version with the same list is verified and reused; one with another list is a conflict. The
//! install touches no running window and no path outside `desktop/versions/` and
//! `desktop/active.json`; after it, [`layout::tend`] places the window entry, moves a flat layout on
//! and removes old window versions. Windows and macOS only, and only where [`window_updates_live`]
//! holds.
pub(super) mod layout;

use super::{
    Context, Error, Output, Settled,
    acquire::Source,
    answer,
    archive::{Download, Unpack},
    feed::{Window, WindowOffer},
    output, own_directory, platform, settle_with, start,
    window_files::{self, Rules, WindowFile, WindowFiles},
};
use crate::{
    invocation::OutputFormat,
    localization::Locale,
    release_store::{ReleaseStore, open_listed_file, open_release_file, read_pointer},
};
use orchbee_local_runtime::{
    EnvironmentKind, compare_product_versions, environment_kind, window_updates_live,
};
use orchbee_release_records::{Entry, Pointer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// The window store's folder in the store root.
pub(super) const FOLDER: &str = "desktop";
/// Whether this platform has window versions; Linux has no window release yet.
const PLATFORM: bool = cfg!(any(windows, target_os = "macos"));

/// What one window part did: the `desktop` member of `orchbee.update`, and the facts of
/// `--installer-window-v1`'s answer.
pub(super) struct WindowReport {
    pub phase: &'static str,
    pub published: Option<PathBuf>,
    pub reused: bool,
    pub activated: bool,
    /// The offered window version is active or lower than the active one, and nothing changed.
    pub up_to_date: bool,
    /// The offered or installed window version, once known.
    pub version: Option<String>,
    /// The window version the pointer named active before this part, once read.
    pub active_before: Option<String>,
    pub cleanup_pending: Option<PathBuf>,
    pub error: Option<Error>,
}

impl WindowReport {
    fn new(version: Option<String>) -> Self {
        Self {
            phase: "manifest",
            published: None,
            reused: false,
            activated: false,
            up_to_date: false,
            version,
            active_before: None,
            cleanup_pending: None,
            error: None,
        }
    }

    /// The `desktop` member of the `orchbee.update` result.
    pub(super) fn document(&self) -> serde_json::Value {
        json!({
            "phase": self.phase, "releasePath": self.published, "reused": self.reused,
            "activated": self.activated, "upToDate": self.up_to_date,
            "releaseVersion": self.version, "cleanupPending": self.cleanup_pending,
            "error": self.error.map(Error::code),
        })
    }
}

/// The window part of one update whose runtime part succeeded, or none when the run has no window
/// part: this build or platform installs no window versions, the store root has no `desktop`
/// folder, the feed offers no window, or the store's active runtime version is lower than the
/// offered window, whose bundled Client would then start its Session from its own folder, so the
/// window waits for a later run. A malformed offer fails the window part alone. Wherever there is
/// a window store, its arrangement follows, whether or not the run had a window part, unless the
/// run was interrupted.
pub(super) fn update(
    root: &Path,
    source: &Source,
    offer: &WindowOffer,
    unpack: Unpack<'_>,
    announce: &dyn Fn(&str, u64),
    context: &mut Context,
) -> Option<WindowReport> {
    if !(PLATFORM && window_updates_live()) {
        return None;
    }
    let desktop = root.join(FOLDER);
    // A link or a file in its place is no window store.
    if !fs::symlink_metadata(&desktop).is_ok_and(|metadata| metadata.is_dir()) {
        return None;
    }
    let report = version_step(root, source, offer, unpack, announce, context);
    if context.check().is_ok() {
        layout::tend(root);
    }
    report
}

/// The window part's version step, or none when the run has no window part.
fn version_step(
    root: &Path,
    source: &Source,
    offer: &WindowOffer,
    unpack: Unpack<'_>,
    announce: &dyn Fn(&str, u64),
    context: &mut Context,
) -> Option<WindowReport> {
    let window = match offer {
        WindowOffer::Absent => return None,
        WindowOffer::Invalid => {
            let mut report = WindowReport::new(None);
            report.error = Some(Error::ManifestInvalid);
            return Some(report);
        }
        WindowOffer::Offered(window) => window,
    };
    let runtime = current_pointer(root)?.active.product;
    if compare_product_versions(&runtime, &window.version) == Ordering::Less {
        return None;
    }
    let mut report = WindowReport::new(Some(window.version.clone()));
    report.error = offered(root, source, window, unpack, announce, context, &mut report).err();
    Some(report)
}

/// What the window store says the part does next.
enum Next {
    /// Up to date: nothing was requested or written.
    Done,
    /// The offered version is stored with the offered list, not active: verify and activate it.
    Stored(WindowFiles),
    /// Download, unpack and install the window archive.
    Download,
}

/// Decides from the feed and the window store alone, then, under `download.lock` and after deciding
/// again, installs the stored copy of the offered window version, or downloads the window archive
/// bounded by its signed size, checks its size and SHA-256, unpacks it, requires its list to be the
/// one the feed binds, and installs that.
fn offered(
    root: &Path,
    source: &Source,
    window: &Window,
    unpack: Unpack<'_>,
    announce: &dyn Fn(&str, u64),
    context: &mut Context,
    report: &mut WindowReport,
) -> Result<(), Error> {
    context.check()?;
    let desktop = root.join(FOLDER);
    if matches!(decide(&desktop, window, context, report)?, Next::Done) {
        return Ok(());
    }
    report.phase = "download";
    let mut download = Download::acquire(root, context, &mut report.cleanup_pending)?;
    let result = (|| {
        // Another update may have finished while this one read the feed.
        let (from, list) = match decide(&desktop, window, context, report)? {
            Next::Done => return Ok(()),
            Next::Stored(list) => (desktop.join("versions").join(&window.version), list),
            Next::Download => {
                let archive = source.artifact(&window.archive.url)?;
                announce(&window.version, window.archive.size);
                let verified = download.fetch(&archive, &window.archive, context)?;
                let unpacked = download.unpack(verified, unpack, context)?;
                report.phase = "verify";
                // The archive's one folder holds the very list the signed feed binds.
                let list = source_list(&unpacked)
                    .filter(|list| list.identity == window.list && list.version == window.version)
                    .ok_or(Error::ManifestInvalid)?;
                (unpacked, list)
            }
        };
        install(root, &from, &list, Lower::Leave, context, report)
    })();
    download.finish(&mut report.cleanup_pending);
    result
}

/// Decides from the window pointer and `desktop/versions/` alone, without a lock, in the runtime's
/// order: the offered version is active with the offered list, or lower than the active one (up to
/// date); it is stored with another list (`version-conflict`); it is stored with the offered list;
/// otherwise it must be downloaded. Nothing is requested or written.
fn decide(
    desktop: &Path,
    window: &Window,
    context: &mut Context,
    report: &mut WindowReport,
) -> Result<Next, Error> {
    let pointer = current_pointer(desktop);
    report.active_before = pointer
        .as_ref()
        .map(|pointer| pointer.active.product.clone());
    let stored = stored_list(desktop, &window.version, context)?;
    if let Some(active) = pointer.map(|pointer| pointer.active) {
        let identity = stored.as_ref().map(|list| list.identity.as_str());
        if active.product == window.version
            && active.manifest == window.list
            && identity == Some(window.list.as_str())
        {
            report.published = Some(desktop.join("versions").join(&window.version));
            report.reused = true;
            report.activated = true;
            report.up_to_date = true;
            report.phase = "complete";
            return Ok(Next::Done);
        }
        // A window version never moves down.
        if compare_product_versions(&active.product, &window.version) == Ordering::Greater {
            report.up_to_date = true;
            report.phase = "complete";
            return Ok(Next::Done);
        }
    }
    match stored {
        None => Ok(Next::Download),
        Some(list) if list.identity == window.list => Ok(Next::Stored(list)),
        Some(_) => Err(Error::VersionConflict),
    }
}

/// What a version above the active window version does to an install.
#[derive(Clone, Copy)]
enum Lower {
    /// `--installer-window-v1`: refused as `downgrade-refused`, as `install` refuses.
    Refuse,
    /// The window part: left alone, up to date.
    Leave,
}

/// Installs the window version `list` names from `from`, the folder that holds its files, into the
/// window store under both writer locks, and makes it the active window version. The decision is
/// taken again under the locks: a higher active version is refused or left as `lower` says, a
/// stored copy with the same list is verified and reused, and one with another list is a conflict.
fn install(
    root: &Path,
    from: &Path,
    list: &WindowFiles,
    lower: Lower,
    context: &mut Context,
    report: &mut WindowReport,
) -> Result<(), Error> {
    context.check()?;
    #[cfg(target_os = "macos")]
    platform::admit_bundle_folders();
    let desktop = root.join(FOLDER);
    // The store root's lock first, as every writer takes it, so one update runs at a time.
    let _root = acquire(root, context)?;
    let mut store = acquire(&desktop, context)?;
    let current = store
        .pointer()
        .map_err(|error| context.storage(&desktop, &error))?
        .and_then(|bytes| Pointer::parse(&bytes));
    report.active_before = current
        .as_ref()
        .map(|pointer| pointer.active.product.clone());
    if current.as_ref().is_some_and(|pointer| {
        compare_product_versions(&pointer.active.product, &list.version) == Ordering::Greater
    }) {
        return match lower {
            Lower::Refuse => Err(Error::DowngradeRefused),
            Lower::Leave => {
                report.up_to_date = true;
                report.phase = "complete";
                Ok(())
            }
        };
    }
    let destination = store
        .version_path(&list.version)
        .map_err(|error| context.storage(&desktop, &error))?;
    if destination
        .try_exists()
        .map_err(|error| context.storage(&destination, &error))?
    {
        // A version is built once, so another list under its name is a conflict.
        if stored_bytes(&destination).ok_or(Error::CohortInvalid)? != list.encoded {
            return Err(Error::VersionConflict);
        }
        report.phase = "verify";
        verify(&destination, list, context)?;
        report.reused = true;
    } else {
        publish(&store, &desktop, from, list, context, report)?;
    }
    report.published = Some(destination);
    report.phase = "activate";
    let entry = Entry {
        product: list.version.clone(),
        manifest: list.identity.clone(),
    };
    if current
        .as_ref()
        .is_none_or(|pointer| pointer.active != entry)
    {
        store
            .replace_pointer(&Pointer::activate(current, entry).encode())
            .map_err(|error| context.storage(&desktop, &error))?;
    }
    report.activated = true;
    report.phase = "complete";
    Ok(())
}

/// Stages the list and every listed file and link, then publishes the stage as
/// `desktop/versions/<version>`. Files come before links, so no file is written through a link.
fn publish(
    store: &ReleaseStore,
    desktop: &Path,
    from: &Path,
    list: &WindowFiles,
    context: &mut Context,
    report: &mut WindowReport,
) -> Result<(), Error> {
    let grammar = Rules::own().grammar();
    let mut stage = store
        .stage()
        .map_err(|error| context.storage(desktop, &error))?;
    let staged = stage.path().to_owned();
    let result = (|| {
        report.phase = "verify";
        {
            // Closed before the stage is renamed, which Windows refuses while a file in it is open.
            let (path, mut file) = stage
                .create_file(window_files::NAME)
                .map_err(|error| context.storage(&staged.join(window_files::NAME), &error))?;
            file.write_all(&list.encoded)
                .and_then(|()| file.sync_all())
                .map_err(|error| context.storage(&path, &error))?;
        }
        for listed in &list.files {
            context.check()?;
            let (path, mut file) = stage
                .create_file_as(grammar, &listed.path)
                .map_err(|error| context.storage(&at(&staged, &listed.path), &error))?;
            let mut input =
                platform::open_source_file(&at(from, &listed.path)).map_err(|_| Error::Checksum)?;
            check(&mut input, listed, Some((&path, &mut file)), context)?;
            if listed.executable {
                stage
                    .mark_executable(&file)
                    .map_err(|error| context.storage(&path, &error))?;
            }
        }
        for link in &list.links {
            stage
                .create_link(&link.path, &link.target)
                .map_err(|error| context.storage(&at(&staged, &link.path), &error))?;
        }
        context.check()?;
        report.phase = "publish";
        stage
            .publish(&list.version)
            .map_err(|error| context.storage(&staged, &error))?;
        Ok(())
    })();
    if stage.cleanup().is_err() {
        report.cleanup_pending = Some(stage.retain_for_recovery());
    }
    result
}

/// Checks every listed file of a stored window version by size and SHA-256, and every listed link
/// by its target.
fn verify(directory: &Path, list: &WindowFiles, context: &mut Context) -> Result<(), Error> {
    let grammar = Rules::own().grammar();
    for listed in &list.files {
        let mut file =
            open_listed_file(directory, &listed.path, grammar).map_err(|_| Error::Checksum)?;
        check(&mut file, listed, None, context)?;
    }
    for link in &list.links {
        let target = fs::read_link(at(directory, &link.path)).map_err(|_| Error::Checksum)?;
        if target != Path::new(&link.target) {
            return Err(Error::Checksum);
        }
    }
    Ok(())
}

/// Reads `input` to its end, at most the listed size, writing each read into `output` when given
/// and flushing it at the end, and requires the listed length and SHA-256.
fn check(
    input: &mut File,
    listed: &WindowFile,
    mut output: Option<(&Path, &mut File)>,
    context: &mut Context,
) -> Result<(), Error> {
    let mut hash = Sha256::new();
    let mut left = listed.size;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        context.check()?;
        let count = input.read(&mut buffer).map_err(|_| Error::Checksum)?;
        if count == 0 {
            break;
        }
        left = u64::try_from(count)
            .ok()
            .and_then(|count| left.checked_sub(count))
            .ok_or(Error::Checksum)?;
        hash.update(&buffer[..count]);
        if let Some((path, file)) = output.as_mut() {
            file.write_all(&buffer[..count])
                .map_err(|error| context.storage(path, &error))?;
        }
    }
    if left != 0 || format!("{:x}", hash.finalize()) != listed.sha256 {
        return Err(Error::Checksum);
    }
    if let Some((path, file)) = output {
        file.sync_all()
            .map_err(|error| context.storage(path, &error))?;
    }
    Ok(())
}

/// Takes the writer lock of the store root `root`; a held lock is another update.
fn acquire(root: &Path, context: &mut Context) -> Result<ReleaseStore, Error> {
    ReleaseStore::acquire(root).map_err(|error| {
        if error.kind() == io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            context.storage(root, &error)
        }
    })
}

/// The pointer of the store root `root`, read without its lock; none when it is missing or
/// unreadable.
fn current_pointer(root: &Path) -> Option<Pointer> {
    read_pointer(root)
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
}

/// The admitted list of the stored window version `version`, read without a lock; none when the
/// version is not stored.
///
/// # Errors
/// `cohort-invalid` when its list cannot be read or admitted, `storage` when its presence cannot
/// be told.
fn stored_list(
    desktop: &Path,
    version: &str,
    context: &mut Context,
) -> Result<Option<WindowFiles>, Error> {
    let directory = desktop.join("versions").join(version);
    if !directory
        .try_exists()
        .map_err(|error| context.storage(&directory, &error))?
    {
        return Ok(None);
    }
    stored_bytes(&directory)
        .and_then(|bytes| WindowFiles::parse(&bytes, Rules::own()))
        .map(Some)
        .ok_or(Error::CohortInvalid)
}

/// The list bytes a stored window version holds, read without following a link.
fn stored_bytes(directory: &Path) -> Option<Vec<u8>> {
    let file = open_release_file(directory, window_files::NAME).ok()?;
    bounded(file)
}

/// The admitted list of the window directory `from`, which an unpacked archive or an installer
/// holds; it need not be private.
fn source_list(from: &Path) -> Option<WindowFiles> {
    let file = platform::open_source_file(&from.join(window_files::NAME)).ok()?;
    WindowFiles::parse(&bounded(file)?, Rules::own())
}

/// At most one byte more than a list may hold, so a longer one is refused by the reader.
fn bounded(file: File) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(window_files::LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

/// The path of the list path `relative` below `directory`, one component at a time.
fn at(directory: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(directory.to_path_buf(), |path, part| path.join(part))
}

/// The window directory the running Client is bundled in: `<window>\resources\cohort\` on Windows
/// and `<window>/<name>.app/Contents/Resources/cohort/` on macOS, where the list sits beside the
/// bundle. A Client anywhere else has none.
fn bundled_window() -> Result<PathBuf, Error> {
    let cohort = own_directory()?;
    let window = if cfg!(target_os = "macos") {
        orchbee_local_runtime::bundle_of_cohort(&cohort).and_then(Path::parent)
    } else {
        let mut folders = cohort.ancestors();
        let named = |folder: Option<&Path>, name: &str| {
            folder
                .and_then(Path::file_name)
                .is_some_and(|own| own == name)
        };
        let shaped = named(folders.next(), "cohort") && named(folders.next(), "resources");
        folders.next().filter(|_| shaped)
    };
    window
        .map(Path::to_path_buf)
        .ok_or(Error::NotReleaseDirectory)
}

/// Whether `install` also makes the window store: on macOS, where window updates run, when the
/// running Client is a window application's bundled one. A copy dragged from the disk image carries
/// no window list, which sits beside the bundle because a file inside it would break its seal, so
/// its version cannot be stored as a window version; the store gains the empty window store
/// instead, and the window part of every later update installs the next window version there.
pub(super) fn install_makes_store() -> bool {
    cfg!(target_os = "macos") && window_updates_live() && bundled_window().is_ok()
}

/// Creates the window store `<root>/desktop/` with its lock and `versions/` when it is missing,
/// under the store root's lock and then its own, as every window install takes them; a store that
/// exists is left as it is.
pub(super) fn make_store(root: &Path, context: &mut Context) -> Result<(), Error> {
    context.check()?;
    let _root = acquire(root, context)?;
    acquire(&root.join(FOLDER), context).map(drop)
}

/// `--installer-window-v1`: installs the window directory the running Client is bundled in, as
/// Setup or install.sh unpacked it, into the window store of this build's store root, creating the
/// `desktop` folder when it is missing, and makes it the active window version; then, whatever the
/// install answered, the window store's arrangement follows unless the run was interrupted. It
/// answers like `install`, with one `orchbee.install` document whatever the outcome: exit 0, or 3
/// with `downgrade-refused` when a higher window version is active and stays active, or with
/// `version-conflict`. It ignores the store's runtime version: a window above it only starts its
/// Session from its own folder. A development build refuses.
pub(crate) fn installer(locale: Locale) -> Output {
    let mut context = match start(
        super::super::execution::Capability::ReleaseInstall,
        OutputFormat::Json,
        locale,
    ) {
        Ok(context) => context,
        Err(output) => return *output,
    };
    let mut report = WindowReport::new(None);
    report.phase = "source";
    let mut store_root = None;
    let result = (|| {
        context.check()?;
        if environment_kind() == EnvironmentKind::Development {
            return Err(Error::DevelopmentBuild);
        }
        let window = bundled_window()?;
        let root = store_root.insert(platform::install_root().map_err(|_| Error::InstallLocation)?);
        report.phase = "manifest";
        let list = source_list(&window).ok_or(Error::ManifestInvalid)?;
        report.version = Some(list.version.clone());
        install(
            root,
            &window,
            &list,
            Lower::Refuse,
            &mut context,
            &mut report,
        )
    })();
    if let Some(root) = store_root.filter(|_| context.check().is_ok()) {
        layout::tend(&root);
    }
    let settled = settle_with(context, None, report.cleanup_pending.is_some(), result);
    answer_installer(&report, settled, locale)
}

/// The helper's whole answer: the `orchbee.install` document, whose runtime-only members are null
/// or `not-requested`, and nothing on standard error.
fn answer_installer(report: &WindowReport, settled: Settled, locale: Locale) -> Output {
    let document = json!({
        "schema": "orchbee.install", "version": 1,
        "result": {
            "phase": report.phase, "channel": null,
            "releaseVersion": report.version, "buildId": null,
            "releasePath": report.published, "reused": report.reused, "activated": report.activated,
            "activeBefore": report.active_before, "pathStep": "not-requested",
            "cleanupPending": report.cleanup_pending,
            "error": settled.error.map(Error::code), "interrupted": settled.interrupted,
        }
    });
    let encoded = output::encode_document(&document);
    answer(encoded, String::new(), settled, OutputFormat::Json, locale)
}
