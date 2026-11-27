//! What the last full check of one cohort found: its manifest bytes and each executable's identity,
//! size and times. A later read-only invocation whose cohort reads the same skips hashing it. The
//! record is a cache in the data root, so a missing, unreadable or different one only means a full
//! check, and a record that cannot be written is left unwritten.
use super::filesystem::stamp;
use crate::{DataRoot, PrivateCache};
use sha2::{Digest, Sha256};
use std::{fs::File, io, path::Path};

/// The folder's one writer lock, `cohort.lock`, guards every cohort's record.
const LOCK: &str = "cohort";
const LIMIT: usize = 4 * 1024;

pub(super) struct Record<'a> {
    root: &'a DataRoot,
    /// The cohort folder's key, which names its staging file, `<key>.pending`.
    key: String,
    document: Vec<u8>,
}

impl<'a> Record<'a> {
    /// The record this cohort's files would leave, or none when a file's stamp cannot be read or
    /// the root carries no admitted layout, since only a root Orchbee already wrote keeps one.
    pub(super) fn new(
        root: &'a DataRoot,
        directory: &Path,
        manifest: &[u8],
        files: &[(String, File)],
    ) -> Option<Self> {
        if !root.carries_layout_record() || root.admit_layout().is_err() {
            return None;
        }
        let key =
            crate::digest_hex::lower_hex(&Sha256::digest(directory.as_os_str().as_encoded_bytes()));
        let mut document = format!(
            "orchbee-cohort-check-v1\nmanifest {}\n",
            crate::digest_hex::lower_hex(&Sha256::digest(manifest))
        );
        for (name, file) in files {
            document.push_str(&format!("{name} {}\n", stamp(file).ok()?));
        }
        Some(Self {
            root,
            key: key[..32].to_owned(),
            document: document.into_bytes(),
        })
    }

    /// Whether the record a full check left reads exactly as this one.
    pub(super) fn recorded(&self) -> bool {
        PrivateCache::read_existing(&self.root.cohort_check_cache(), &self.file_name(), LIMIT)
            .is_ok_and(|bytes| bytes.as_deref() == Some(self.document.as_slice()))
    }

    /// Records a full check that passed; a writer that holds the lock already records the same.
    pub(super) fn store(&self) {
        let _: io::Result<()> =
            PrivateCache::acquire_with_lock(&self.root.cohort_check_cache(), LOCK, &self.key)
                .and_then(|mut cache| cache.replace(&self.file_name(), &self.document));
    }

    fn file_name(&self) -> String {
        format!("{}.txt", self.key)
    }
}
