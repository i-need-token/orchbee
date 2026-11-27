//! Immutable cohort admission and artifact identities; filesystem adapters own handle-level checks.
use crate::DataRoot;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    time::Instant,
};

mod filesystem;
mod record;
use filesystem::open_file;

const MANIFEST_LIMIT: u64 = 16 * 1024;
const SCHEMA_VERSION: u32 = 1;
const OBSERVATION_ADAPTERS: &str = "agent-observation-adapters";
const ARTIFACTS: [CohortArtifact; 4] = [
    CohortArtifact::Client,
    CohortArtifact::Server,
    CohortArtifact::Worker,
    CohortArtifact::TerminalHost,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CohortArtifact {
    Client,
    Server,
    Worker,
    TerminalHost,
}
impl CohortArtifact {
    /// The file name this artifact has in its cohort directory. Environments ship distinct names
    /// (ORC-382), so callers take the name from here rather than spelling one.
    pub fn file_name(self) -> String {
        filesystem::artifact_file_name(self)
    }
    /// Whether this build's cohort carries the shipped release names rather than a development or
    /// test environment stem. Only such a build may act on the release installation.
    pub fn has_release_names() -> bool {
        filesystem::has_release_names()
    }
}

/// A native cohort admission failure with the exact required artifact when
/// opening that sibling reported `NotFound`.
#[derive(Debug)]
pub struct CohortError {
    source: io::Error,
    missing_artifact: Option<CohortArtifact>,
}
impl CohortError {
    fn new(source: io::Error, missing_artifact: Option<CohortArtifact>) -> Self {
        Self {
            source,
            missing_artifact,
        }
    }

    /// The observed operating-system error class.
    pub fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }

    /// The observed raw operating-system error code, when available.
    pub fn raw_os_error(&self) -> Option<i32> {
        self.source.raw_os_error()
    }

    /// The required sibling proven absent by the artifact-open operation.
    pub fn missing_artifact(&self) -> Option<CohortArtifact> {
        self.missing_artifact
    }

    /// Recover the original I/O error for callers that only use its standard
    /// error interface.
    pub fn into_io_error(self) -> io::Error {
        self.source
    }
}
impl std::fmt::Display for CohortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, formatter)
    }
}
impl std::error::Error for CohortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
impl From<io::Error> for CohortError {
    fn from(source: io::Error) -> Self {
        Self::new(source, None)
    }
}

#[derive(Debug)]
pub struct Cohort {
    directory: PathBuf,
    identity: String,
    artifact_hashes: [String; 4],
    observation_adapters: ObservationAdaptersSidecar,
    /// The digest the manifest declares for `orchbee-files`, which only development and test
    /// cohorts carry.
    files: Option<String>,
}

/// Why the cohort's `orchbee-files` cannot be started.
#[derive(Debug)]
pub enum FilesExecutableError {
    /// The manifest declares no `orchbee-files`, or the cohort holds no such file.
    Missing,
    /// The file is not one only this user or root can change, differs from the declared digest,
    /// or could not be read before the deadline.
    Unverified(io::Error),
}

/// The observation-adapters payload a current cohort manifest declares. Startup
/// never opens, traverses, or validates it; the first integration consumer
/// validates the directory privately against the declared digest and answers a
/// typed unavailable/invalid result, so an absent or damaged sidecar cannot
/// block Session startup or Agent reads.
#[derive(Debug)]
pub struct ObservationAdaptersSidecar {
    directory: PathBuf,
    declared_payload_sha256: String,
}
impl ObservationAdaptersSidecar {
    /// The fixed sibling directory the staged payload occupies.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// The lowercase SHA-256 payload digest the manifest declares.
    pub fn declared_payload_sha256(&self) -> &str {
        &self.declared_payload_sha256
    }
}

/// Reject a different version before checking the complete current manifest shape.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestVersion {
    schema_version: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    #[allow(dead_code)]
    schema_version: u32,
    artifacts: Artifacts,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifacts {
    orchbee: String,
    #[serde(rename = "orchbee-server")]
    server: String,
    #[serde(rename = "orchbee-worker")]
    worker: String,
    #[serde(rename = "orchbee-terminal-host")]
    host: String,
    #[serde(rename = "agent-observation-adapters")]
    observation_adapters: String,
    /// Optional, so a release manifest, which lists no `orchbee-files`, keeps its shape.
    #[serde(rename = "orchbee-files", default)]
    files: Option<String>,
}
impl Artifacts {
    fn hash(&self, artifact: CohortArtifact) -> &str {
        match artifact {
            CohortArtifact::Client => &self.orchbee,
            CohortArtifact::Server => &self.server,
            CohortArtifact::Worker => &self.worker,
            CohortArtifact::TerminalHost => &self.host,
        }
    }
}

impl Cohort {
    pub fn current(deadline: Instant) -> io::Result<Self> {
        Self::current_with_diagnostic(deadline).map_err(CohortError::into_io_error)
    }

    /// Validate this executable's sibling cohort while retaining an exact
    /// missing artifact when a required executable was observed absent.
    ///
    /// # Errors
    /// Returns the underlying I/O or validation failure. `missing_artifact`
    /// is populated only when opening one of the declared native siblings
    /// returned `NotFound`.
    pub fn current_with_diagnostic(deadline: Instant) -> Result<Self, CohortError> {
        let executable = current_executable()?;
        let directory = executable
            .parent()
            .ok_or_else(|| CohortError::from(invalid()))?;
        Self::load_with_diagnostic(directory, deadline)
    }

    pub fn load(directory: &Path, deadline: Instant) -> io::Result<Self> {
        Self::load_with_diagnostic(directory, deadline).map_err(CohortError::into_io_error)
    }

    /// Validate a cohort in `directory`, retaining the exact required
    /// artifact when opening a sibling executable reports `NotFound`.
    ///
    /// # Errors
    /// Returns the underlying I/O or validation failure. `missing_artifact`
    /// is populated only for a missing declared native sibling.
    pub fn load_with_diagnostic(directory: &Path, deadline: Instant) -> Result<Self, CohortError> {
        Self::admit(directory, deadline, None)
    }

    /// Like [`Self::current_with_diagnostic`], for an invocation that only reads: a cohort whose
    /// manifest and executables read exactly as `root` recorded them at the last full check is
    /// admitted without hashing them again. Any difference, or no record, means a full check, which
    /// records what it found.
    ///
    /// # Errors
    /// As [`Self::current_with_diagnostic`].
    pub fn current_remembered(root: &DataRoot, deadline: Instant) -> Result<Self, CohortError> {
        let executable = current_executable()?;
        let directory = executable
            .parent()
            .ok_or_else(|| CohortError::from(invalid()))?;
        Self::load_remembered(root, directory, deadline)
    }

    pub(crate) fn load_remembered(
        root: &DataRoot,
        directory: &Path,
        deadline: Instant,
    ) -> Result<Self, CohortError> {
        Self::admit(directory, deadline, Some(root))
    }

    fn admit(
        directory: &Path,
        deadline: Instant,
        remembered: Option<&DataRoot>,
    ) -> Result<Self, CohortError> {
        remaining(deadline)?;
        let descriptor = filesystem::open_directory(directory)?;
        let mut encoded = Vec::new();
        open_file(
            &descriptor,
            "cohort.json",
            false,
            &directory.join("cohort.json"),
        )?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut encoded)?;
        if encoded.len() as u64 > MANIFEST_LIMIT {
            return Err(invalid().into());
        }
        let version: ManifestVersion = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
        if version.schema_version != SCHEMA_VERSION {
            return Err(incompatible().into());
        }
        let manifest: Manifest = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
        if !declared_hash(&manifest.artifacts.observation_adapters)
            || manifest
                .artifacts
                .files
                .as_deref()
                .is_some_and(|hash| !declared_hash(hash))
        {
            return Err(invalid().into());
        }
        let mut files = Vec::with_capacity(ARTIFACTS.len());
        for artifact in ARTIFACTS {
            if !declared_hash(manifest.artifacts.hash(artifact)) {
                return Err(invalid().into());
            }
            let name = artifact.file_name();
            let file =
                open_file(&descriptor, &name, true, &directory.join(&name)).map_err(|source| {
                    let missing_artifact =
                        (source.kind() == io::ErrorKind::NotFound).then_some(artifact);
                    CohortError::new(source, missing_artifact)
                })?;
            files.push((name, file));
        }
        let record =
            remembered.and_then(|root| record::Record::new(root, directory, &encoded, &files));
        if !record.as_ref().is_some_and(record::Record::recorded) {
            for (artifact, (_, file)) in ARTIFACTS.into_iter().zip(&mut files) {
                let mut hash = Sha256::new();
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    remaining(deadline)?;
                    let count = file.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    hash.update(&buffer[..count]);
                }
                if crate::digest_hex::lower_hex(&hash.finalize())
                    != manifest.artifacts.hash(artifact)
                {
                    return Err(invalid().into());
                }
            }
            if let Some(record) = &record {
                record.store();
            }
        }
        let mut identity = Sha256::new();
        identity.update(b"orchbee-native-cohort-v1\n");
        for (artifact, (name, _)) in ARTIFACTS.into_iter().zip(&files) {
            let expected = manifest.artifacts.hash(artifact);
            identity.update(name.as_bytes());
            identity.update([0]);
            identity.update(expected.as_bytes());
            identity.update(b"\n");
        }
        remaining(deadline)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            identity: crate::digest_hex::lower_hex(&identity.finalize()),
            artifact_hashes: ARTIFACTS.map(|artifact| manifest.artifacts.hash(artifact).to_owned()),
            observation_adapters: ObservationAdaptersSidecar {
                directory: directory.join(OBSERVATION_ADAPTERS),
                declared_payload_sha256: manifest.artifacts.observation_adapters,
            },
            files: manifest.artifacts.files,
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    /// The digest independently verified during this cohort admission.
    /// Comparing retained observations never rereads mutable manifest bytes.
    pub fn artifact_sha256(&self, artifact: CohortArtifact) -> &str {
        &self.artifact_hashes[match artifact {
            CohortArtifact::Client => 0,
            CohortArtifact::Server => 1,
            CohortArtifact::Worker => 2,
            CohortArtifact::TerminalHost => 3,
        }]
    }
    pub fn executable(&self, artifact: CohortArtifact) -> PathBuf {
        self.directory.join(artifact.file_name())
    }
    /// The lazily admitted observation-adapters declaration: path and declared
    /// digest only — never an opened or validated payload.
    pub fn observation_adapters_sidecar(&self) -> &ObservationAdaptersSidecar {
        &self.observation_adapters
    }

    /// The file name of the cohort's optional `orchbee-files` in this build's environment.
    #[must_use]
    pub fn files_file_name() -> String {
        filesystem::files_file_name()
    }

    /// The file name of the rg a development or test cohort carries beside `orchbee-files`.
    #[must_use]
    pub fn ripgrep_file_name() -> &'static str {
        if cfg!(windows) { "rg.exe" } else { "rg" }
    }

    /// The cohort's `orchbee-files`, opened as the cohort's own executables are and hashed against
    /// the digest the manifest declares, so a start runs only the program this cohort shipped.
    /// Admission never checks it: a release cohort carries none, and a damaged one fails only the
    /// File Bar.
    ///
    /// # Errors
    ///
    /// [`FilesExecutableError::Missing`] when the manifest declares none or no such file exists,
    /// and [`FilesExecutableError::Unverified`] for any other failure.
    pub fn verified_files_executable(
        &self,
        deadline: Instant,
    ) -> Result<PathBuf, FilesExecutableError> {
        let declared = self.files.as_deref().ok_or(FilesExecutableError::Missing)?;
        let name = filesystem::files_file_name();
        let path = self.directory.join(&name);
        let unverified = |error: io::Error| match error.kind() {
            io::ErrorKind::NotFound => FilesExecutableError::Missing,
            _ => FilesExecutableError::Unverified(error),
        };
        let descriptor = filesystem::open_directory(&self.directory).map_err(unverified)?;
        let mut file = open_file(&descriptor, &name, true, &path).map_err(unverified)?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            remaining(deadline).map_err(FilesExecutableError::Unverified)?;
            let count = file
                .read(&mut buffer)
                .map_err(FilesExecutableError::Unverified)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if crate::digest_hex::lower_hex(&hash.finalize()) != declared {
            return Err(FilesExecutableError::Unverified(invalid()));
        }
        Ok(path)
    }
}

/// This process's program file. macOS reports the path the program was started through, which is
/// the link's own path when a link such as `~/.local/bin/orchbee` started it, so there the path is
/// resolved and the program's cohort is found beside the file itself. Linux already reports the
/// resolved file, and Windows starts no program through a link.
pub fn current_executable() -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "macos")]
    let executable = std::fs::canonicalize(executable)?;
    Ok(executable)
}

fn remaining(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
fn declared_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid native development cohort",
    )
}
fn incompatible() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "incompatible native cohort manifest version",
    )
}

#[cfg(all(test, unix))]
mod tests;
