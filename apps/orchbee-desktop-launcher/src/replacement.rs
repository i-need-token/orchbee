//! The macOS window launcher's decisions. The application bundle is the entry on macOS, so the
//! launcher runs only when a window relaunches: from the cohort of a window version's bundle in the
//! store, `<desktop>/versions/<version>/<name>.app/Contents/Resources/cohort/`, started by the window
//! that then quits. It decides whether the request is a relaunch it may serve, which application it
//! replaces, which window version from the store replaces it, and the arguments of each start.
//! Like `decision`, it reads no clock, starts nothing, and sees the file system only through the
//! facts its callers pass in.
use crate::decision::{self, FALLBACK_FROM, POINTER, RELAUNCH_AFTER, VERSIONS};
use orchbee_release_records::{CrossBuildName, Pointer};
use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[cfg(test)]
mod tests;

/// Prefixed to the application bundle the asking window runs from, as the argument after
/// `--relaunch-after <process id>`: the application the launcher replaces. A window up to
/// 0.1.0-beta.24 sends the earlier spelling.
const REPLACE_APPLICATION: CrossBuildName = CrossBuildName {
    current: "--replace-application=",
    earlier: "--replace-application-v1=",
};
/// The folders from a window version's bundle down to the launcher, outermost first.
const COHORT: [&str; 3] = ["Contents", "Resources", "cohort"];
/// The folders from an application bundle down to its program.
const PROGRAM: [&str; 2] = ["Contents", "MacOS"];
const BUNDLE_EXTENSION: &str = "app";

/// A relaunch the launcher serves: its parent asked for it and named the application it ran from.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Relaunch {
    /// The asking window, the launcher's parent; nothing changes before it ends.
    pub(crate) wait_for: u32,
    /// The application bundle the window ran from, which the launcher replaces.
    pub(crate) application: PathBuf,
    /// The window program inside any bundle of this product, `Contents/MacOS/<name>`.
    pub(crate) program: PathBuf,
    /// The arguments every window start passes on, after the launcher's own.
    pub(crate) arguments: Vec<OsString>,
    /// The spelling of `--fallback-from=` the window that ran before reads: the one it asked in.
    fallback_from: &'static str,
    /// `<desktop>`, which holds the window pointer and `versions`.
    desktop: PathBuf,
    /// The bundle name every window version's folder holds, `<name>.app`.
    bundle: OsString,
}

/// The active window version and its bundle in the store.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Source {
    pub(crate) version: String,
    pub(crate) bundle: PathBuf,
}

/// The relaunch the launcher at `launcher` serves for `arguments` after the program name, when
/// `parent` is its parent process: the arguments must start with `--relaunch-after <parent>`
/// and then `--replace-application=<application>`, the launcher must sit in a window version's
/// bundle in the store, and the application must be an absolute `.app` path outside the store's
/// `desktop` folder that spells no `.` or `..` component. `None` serves nothing.
pub(crate) fn relaunch(launcher: &Path, arguments: Vec<OsString>, parent: u32) -> Option<Relaunch> {
    let (desktop, bundle) = store_layout(launcher)?;
    let fallback_from = if arguments
        .first()
        .is_some_and(|first| first == RELAUNCH_AFTER.earlier)
    {
        FALLBACK_FROM.earlier
    } else {
        FALLBACK_FROM.current
    };
    let (relaunch_after, mut arguments) = decision::own_arguments(arguments);
    if relaunch_after != Some(parent) {
        return None;
    }
    let application = arguments
        .first()
        .and_then(|first| {
            let first = first.as_bytes();
            first
                .strip_prefix(REPLACE_APPLICATION.current.as_bytes())
                .or_else(|| first.strip_prefix(REPLACE_APPLICATION.earlier.as_bytes()))
        })
        .map(|value| PathBuf::from(OsStr::from_bytes(value)))?;
    arguments.remove(0);
    let spelled = application.as_os_str().as_bytes();
    let inside_store = {
        let desktop = desktop.as_os_str().as_bytes().to_ascii_lowercase();
        let lowered = spelled.to_ascii_lowercase();
        lowered
            .strip_prefix(desktop.as_slice())
            .is_some_and(|rest| rest.first() == Some(&b'/'))
    };
    if !application.is_absolute()
        || application.extension() != Some(OsStr::new(BUNDLE_EXTENSION))
        || spelled
            .split(|byte| *byte == b'/')
            .any(|part| part == b"." || part == b"..")
        || inside_store
    {
        return None;
    }
    let mut program = PROGRAM.iter().collect::<PathBuf>();
    program.push(Path::new(&bundle).file_stem()?);
    Some(Relaunch {
        wait_for: parent,
        application,
        program,
        arguments,
        fallback_from,
        desktop,
        bundle,
    })
}

impl Relaunch {
    /// The window pointer, which `source` reads.
    pub(crate) fn pointer(&self) -> PathBuf {
        self.desktop.join(POINTER)
    }

    /// The active window version that `pointer` names in `bytes`, when its program exists by
    /// `exists`; `None` when the pointer names none or that bundle cannot start.
    pub(crate) fn source(
        &self,
        bytes: Option<Vec<u8>>,
        exists: impl Fn(&Path) -> bool,
    ) -> Option<Source> {
        let version = Pointer::parse(&bytes?)?.active.product;
        let bundle = self
            .desktop
            .join(VERSIONS)
            .join(&version)
            .join(&self.bundle);
        exists(&bundle.join(&self.program)).then_some(Source { version, bundle })
    }

    /// The arguments of the window that ran before, started again after `failed` could not
    /// replace it or failed inside the watch: `--fallback-from=<failed>`, in the spelling that
    /// window reads, first when one is known.
    pub(crate) fn fallback_arguments(&self, failed: Option<&str>) -> Vec<OsString> {
        failed.map_or_else(
            || self.arguments.clone(),
            |version| decision::fallback_arguments(self.fallback_from, version, &self.arguments),
        )
    }
}

/// `<desktop>` and `<name>.app` when `launcher` is
/// `<desktop>/versions/<version>/<name>.app/Contents/Resources/cohort/<file>`.
fn store_layout(launcher: &Path) -> Option<(PathBuf, OsString)> {
    let mut folder = launcher.parent()?;
    for name in COHORT.iter().rev() {
        if folder.file_name() != Some(OsStr::new(name)) {
            return None;
        }
        folder = folder.parent()?;
    }
    let bundle = folder.file_name()?;
    if Path::new(bundle).extension() != Some(OsStr::new(BUNDLE_EXTENSION)) {
        return None;
    }
    let version = folder.parent()?;
    let versions = version.parent()?;
    let valid_version = version
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(orchbee_release_records::valid_version);
    if !valid_version || versions.file_name() != Some(OsStr::new(VERSIONS)) {
        return None;
    }
    Some((versions.parent()?.to_path_buf(), bundle.to_os_string()))
}
