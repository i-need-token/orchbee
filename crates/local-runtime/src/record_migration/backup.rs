//! Effects of migration: upgrade backups below the data root's
//! `internal/backups/`, each a private folder holding copies at the records' paths relative to the
//! root.
//!
//! Cross-runtime contract: search for `upgrade-backup-folders` before changing the upgrade folder
//! name, the relative path, the numbered copies, or retention; Desktop Main backs its window state
//! record up into the same folders.
use super::{RecordNotice, VersionedRecord, version_folder_name};
use crate::{
    DataRoot, PRODUCT_VERSION, data_root::compare_product_versions,
    private_filesystem as permissions,
};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// How many upgrade folders the root keeps; creating another deletes the older ones.
pub(crate) const KEPT_UPGRADE_FOLDERS: usize = 3;
/// Names taken beyond this many in one folder leave the record unchanged instead of searching on.
const NAME_LIMIT: u32 = 1000;
const UPGRADE: &str = "-upgrade-";

/// Persists a migration that a writer computed under its own writer lock. The original is copied
/// into `internal/backups/<YYYY-MM-DD>-upgrade-<product version>/` at the record's path relative
/// to the root and flushed; every record one product version migrates shares that folder. Only
/// then does `replace`, the store's flushed atomic replacement, write the migrated record. Any
/// failure leaves the record at its path unchanged. Answers the backup copy's path.
pub(crate) fn persist_migration(
    root: &DataRoot,
    path: &Path,
    original: &[u8],
    replace: impl FnOnce() -> io::Result<()>,
) -> io::Result<PathBuf> {
    let folder = upgrade_folder(
        &root.backups(),
        &crate::diagnostic_log::local_date(),
        PRODUCT_VERSION,
    )?;
    let copy = copy_into(root, &folder, path, original)?;
    replace()?;
    Ok(copy)
}

/// Reads the device records, the plugin registry and the records of each Session folder, or only
/// of `session` when given, creating nothing, and reports each one whose version this build cannot
/// use. `config check` lists them, and a Client shows what the Server's records hold.
pub fn inspect_records(root: &DataRoot, session: Option<&str>) -> Vec<RecordNotice> {
    let mut found = vec![
        (VersionedRecord::Machines, root.state(), "machines.json"),
        (
            VersionedRecord::PluginRegistry,
            root.plugins(),
            "registry.json",
        ),
    ];
    let folders: Vec<PathBuf> = match session {
        Some(session) => vec![root.sessions().join(session)],
        None => fs::read_dir(root.sessions())
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .map(|entry| entry.path())
                    .collect()
            })
            .unwrap_or_default(),
    };
    for folder in folders {
        found.push((
            VersionedRecord::SessionLifecycle,
            folder.clone(),
            "session.json",
        ));
        found.push((VersionedRecord::TuiPosition, folder, "tui-position.json"));
    }
    let mut notices = Vec::new();
    for (record, folder, file) in found {
        let Ok(Some(bytes)) = crate::PrivateCache::read_existing(&folder, file, INSPECTED_LIMIT)
        else {
            continue;
        };
        if let Some((problem, version)) = super::open_record(record, &bytes, |_| true).problem() {
            let notice = RecordNotice {
                record,
                problem,
                version,
                path: folder.join(file),
            };
            super::report_record_notice(&notice);
            notices.push(notice);
        }
    }
    notices
}

/// Larger than every versioned record's own bound, so inspection reads each one whole.
const INSPECTED_LIMIT: usize = 1024 * 1024;

/// The upgrade folder of `version`: an existing `<date>-upgrade-<version>` folder, or a new one
/// named by `date`, after which only the [`KEPT_UPGRADE_FOLDERS`] newest upgrade folders stay.
pub(crate) fn upgrade_folder(backups: &Path, date: &str, version: &str) -> io::Result<PathBuf> {
    let _pins = crate::data_root::create_state_directory(backups)?;
    let version = version_folder_name(version);
    let mut folders = upgrade_folders(backups)?;
    if let Some((_, _, name)) = folders.iter().find(|(_, held, _)| *held == version) {
        return Ok(backups.join(name));
    }
    let name = format!("{date}{UPGRADE}{version}");
    let folder = backups.join(&name);
    let _created = crate::data_root::create_state_directory(&folder)?;
    folders.push((date.to_owned(), version, name));
    folders.sort_by(|(left_date, left, _), (right_date, right, _)| {
        right_date
            .cmp(left_date)
            .then_with(|| compare_product_versions(right, left))
    });
    for (_, _, name) in folders.iter().skip(KEPT_UPGRADE_FOLDERS) {
        // A folder that cannot go stays until the next upgrade folder is created.
        let _ = fs::remove_dir_all(backups.join(name));
    }
    Ok(folder)
}

/// Every `<YYYY-MM-DD>-upgrade-<version>` folder below `backups`, as date, version and name.
fn upgrade_folders(backups: &Path) -> io::Result<Vec<(String, String, String)>> {
    let mut folders = Vec::new();
    for entry in fs::read_dir(backups)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Some((date, version)) = name.split_at_checked(10).and_then(|(date, rest)| {
            let version = rest
                .strip_prefix(UPGRADE)
                .filter(|version| !version.is_empty())?;
            dated(date).then_some((date.to_owned(), version.to_owned()))
        }) else {
            continue;
        };
        folders.push((date, version, name));
    }
    Ok(folders)
}

/// Whether `text` is spelled `YYYY-MM-DD`.
fn dated(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn numbered(stem: &str, number: u32) -> String {
    if number <= 1 {
        stem.to_owned()
    } else {
        format!("{stem}-{number}")
    }
}

/// Writes `original` into `folder` at `path`'s place relative to `root` and flushes it. A copy
/// already there with the same content is kept; different content takes the next free `-2`, `-3`
/// and so on before the extension, so the first original a version backed up is never replaced.
fn copy_into(root: &DataRoot, folder: &Path, path: &Path, original: &[u8]) -> io::Result<PathBuf> {
    let relative = path
        .strip_prefix(root.path())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let target = folder.join(relative);
    let parent = target.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let _pins = crate::data_root::create_state_directory(parent)?;
    let leaf = target
        .file_name()
        .and_then(|leaf| leaf.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let (stem, extension) = leaf
        .rsplit_once('.')
        .map_or((leaf, String::new()), |(stem, extension)| {
            (stem, format!(".{extension}"))
        });
    for number in 1..=NAME_LIMIT {
        let candidate = parent.join(format!("{}{extension}", numbered(stem, number)));
        match permissions::open_file(&candidate, true, true) {
            Ok(mut file) => {
                let written = file.write_all(original).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = written {
                    let _ = fs::remove_file(&candidate);
                    return Err(error);
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let mut held = Vec::new();
                permissions::open_file(&candidate, false, false)?
                    .take(original.len() as u64 + 1)
                    .read_to_end(&mut held)?;
                if held == original {
                    return Ok(candidate);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}
