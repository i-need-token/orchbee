//! The repositories above a Workspace root, or holding a file opened outside it, that the user
//! allowed Source Control to show: `internal/state/git-parent-repositories.json`, `{version: 1,
//! repositories}`, one canonical toplevel path per repository, per machine. Every process of the
//! machine reads it at each discovery and replaces it atomically under its writer lock, so an
//! answer given in one Session holds in all of them. A record this build cannot use is reported
//! once and never rewritten; answers then last only as long as this process.

use super::native_paths;
use orchbee_local_runtime::{DataRoot, OpenedRecord, VersionedFile, VersionedRecord};
use serde::{Deserialize, Serialize};
use std::io;

const MAXIMUM_BYTES: usize = 1024 * 1024;
/// The most repositories the record keeps; the oldest answers leave first.
const MAXIMUM_REPOSITORIES: usize = 1024;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u64,
    repositories: Vec<String>,
}

fn admits(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Record>(bytes).is_ok()
}

/// The machine's consent record, or none when this process has no data root.
#[derive(Clone, Debug)]
pub(super) struct Consents {
    file: Option<VersionedFile>,
}

impl Consents {
    pub(super) fn new(root: Option<DataRoot>) -> Self {
        Self {
            file: root
                .map(|root| VersionedFile::device(root, VersionedRecord::GitParentRepositories)),
        }
    }

    /// The consented repositories, an absent record consenting to none; `None` without a data
    /// root or when the record cannot be read, so the answers this process holds stay.
    pub(super) fn read(&self) -> Option<Vec<String>> {
        let file = self.file.as_ref()?;
        match file.read(MAXIMUM_BYTES, admits) {
            Ok(OpenedRecord::Readable(bytes)) => serde_json::from_slice::<Record>(&bytes)
                .map(|record| record.repositories)
                .ok(),
            Ok(OpenedRecord::Absent) => Some(Vec::new()),
            Ok(OpenedRecord::Unusable(_)) | Err(_) => None,
        }
    }

    /// Records the user's answer for `repository`: consented, or closed again. Without a data
    /// root nothing records it.
    pub(super) fn answer(&self, repository: &str, consented: bool) -> io::Result<()> {
        let file = self.file.as_ref().ok_or(io::ErrorKind::Unsupported)?;
        let (mut writer, opened) = file.acquire(MAXIMUM_BYTES, admits)?;
        let mut repositories = match opened {
            Ok(OpenedRecord::Readable(bytes)) => serde_json::from_slice::<Record>(&bytes)
                .map(|record| record.repositories)
                .unwrap_or_default(),
            Ok(OpenedRecord::Absent) => Vec::new(),
            Ok(OpenedRecord::Unusable(_)) => return Err(io::ErrorKind::Unsupported.into()),
            Err(error) => return Err(error),
        };
        repositories.retain(|kept| !native_paths::same(kept, repository));
        if consented {
            repositories.push(repository.to_owned());
            let excess = repositories.len().saturating_sub(MAXIMUM_REPOSITORIES);
            repositories.drain(..excess);
        }
        let bytes = serde_json::to_vec_pretty(&Record {
            version: VersionedRecord::GitParentRepositories.current_version(),
            repositories,
        })
        .map_err(io::Error::other)?;
        writer.replace(&bytes)
    }
}
